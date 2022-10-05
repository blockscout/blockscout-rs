import { Link, Icon, Text, HStack, Tooltip, Box } from '@chakra-ui/react';
import NextLink from 'next/link';
import React from 'react';
import styles from '../styles/header.module.css'
import { HeaderLink } from './HeaderLink';

export const Header = () => {
    return (
        <div className={styles.header}>
        <Link href="/"> <h1>
          Blockscout Multi Search
        </h1></Link>
        <div className={styles.right}>
          <HeaderLink url="https://blockscout.com" text="Blockscout"/>
          <HeaderLink url="/about" text="About"/>
        </div>
      </div>
    )
}