import Head from 'next/head'
import React from 'react'
import { Header } from './Header'
import { SearchBar } from './SearchBar'
import styles from '../styles/search.module.css'

interface Props {
  children?: React.ReactNode,
  initialSearchValue: string,
}


export const Search = ({ children, initialSearchValue}: Props) => {
  return (
      <>
      <Head>
        <title>Blockscout Multi Search</title>
        <link rel="icon" href="/favicon.ico"/>
      </Head>
      <div className={styles.main}>
        <Header/>
        <div className={styles.content}>
          <SearchBar initialValue={initialSearchValue}></SearchBar>
          {children}
        </div>
      </div>
      </>
  )
}



